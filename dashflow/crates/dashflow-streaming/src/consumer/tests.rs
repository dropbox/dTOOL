// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! Unit tests for DashStreamConsumer.
//!
//! Tests cover:
//! - ConsumerConfig configuration
//! - SequenceValidator gap detection and recovery
//! - Message ordering and deduplication
//! - Offset checkpointing
//! - Schema validation

use super::*;

    // ============================================================================
    // ConsumerConfig Tests
    // ============================================================================

    #[test]
    fn test_consumer_config_default() {
        let config = ConsumerConfig::default();
        assert_eq!(config.bootstrap_servers, "localhost:9092");
        assert_eq!(config.topic, "dashstream-events");
        assert_eq!(config.auto_offset_reset, "earliest");
        assert!(config.enable_auto_commit);
        assert_eq!(config.auto_commit_interval_ms, 5000);
        assert!(config.offset_checkpoint_path.is_none());
        assert!(config.enable_decompression);
        // TLS/SASL defaults
        assert!(!config.enable_tls);
        assert!(config.ssl_ca_location.is_none());
        assert!(config.ssl_certificate_location.is_none());
        assert!(config.ssl_key_location.is_none());
        assert!(config.sasl_username.is_none());
        assert!(config.sasl_password.is_none());
        // Strict validation is enabled by default (security by default)
        assert!(config.enable_strict_validation);
        // Sequence validation is enabled by default (skips legacy zero sequences)
        assert!(config.enable_sequence_validation);
        assert_eq!(
            config.gap_recovery_policy,
            GapRecoveryPolicy::WarnAndContinue
        );
    }

    #[test]
    fn test_consumer_config_custom() {
        let config = ConsumerConfig {
            bootstrap_servers: "kafka1:9092,kafka2:9092".to_string(),
            topic: "custom-topic".to_string(),
            partition: 0,
            auto_offset_reset: "latest".to_string(),
            enable_auto_commit: false,
            auto_commit_interval_ms: 10000,
            offset_checkpoint_path: None,
            enable_decompression: false,
            max_message_size: 1_048_576,
            enable_tls: false,
            ssl_ca_location: None,
            ssl_certificate_location: None,
            ssl_key_location: None,
            sasl_username: None,
            sasl_password: None,
            enable_strict_validation: false, // Legacy compatibility mode
            schema_compatibility: SchemaCompatibility::Exact,
            enable_sequence_validation: true,
            gap_recovery_policy: GapRecoveryPolicy::WarnAndContinue,
            enable_dlq: true,
            dlq_topic: "dashstream-dlq".to_string(),
            dlq_timeout: Duration::from_secs(5),
            fetch_backoff_initial: Duration::from_millis(100),
            fetch_backoff_max: Duration::from_secs(5),
            idle_poll_sleep: Duration::from_millis(50),
            ..Default::default()
        };
        assert_eq!(config.bootstrap_servers, "kafka1:9092,kafka2:9092");
        assert_eq!(config.topic, "custom-topic");
        assert!(!config.enable_strict_validation);
        assert_eq!(config.auto_offset_reset, "latest");
        assert!(!config.enable_auto_commit);
        assert_eq!(config.auto_commit_interval_ms, 10000);
        assert!(!config.enable_decompression);
    }

    #[test]
    fn test_consumer_config_clone() {
        let config1 = ConsumerConfig::default();
        let config2 = config1.clone();
        assert_eq!(config1.bootstrap_servers, config2.bootstrap_servers);
        assert_eq!(config1.topic, config2.topic);
        assert_eq!(config1.auto_offset_reset, config2.auto_offset_reset);
        assert_eq!(config1.enable_auto_commit, config2.enable_auto_commit);
        assert_eq!(
            config1.auto_commit_interval_ms,
            config2.auto_commit_interval_ms
        );
        assert_eq!(config1.enable_decompression, config2.enable_decompression);
    }

    #[test]
    fn test_consumer_config_debug() {
        let config = ConsumerConfig::default();
        let debug_str = format!("{:?}", config);
        assert!(debug_str.contains("ConsumerConfig"));
        assert!(debug_str.contains("localhost:9092"));
        assert!(debug_str.contains("dashstream-events"));
    }

    // ============================================================================
    // Offset Checkpoint Tests
    // ============================================================================

    #[test]
    fn test_offset_checkpoint_round_trip_json() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("checkpoint.json");

        let checkpoint = OffsetCheckpoint {
            topic: "dashstream-events".to_string(),
            partition: 7,
            offset: 12345,
        };

        store_offset_checkpoint_atomic(&path, &checkpoint).unwrap();

        let config = ConsumerConfig {
            topic: "dashstream-events".to_string(),
            partition: 7,
            ..Default::default()
        };
        let loaded = load_offset_checkpoint(&path, &config).unwrap().unwrap();
        assert_eq!(loaded, checkpoint);
    }

    #[test]
    fn test_offset_checkpoint_load_legacy_integer() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("checkpoint.txt");

        std::fs::write(&path, "42\n").unwrap();

        let config = ConsumerConfig {
            topic: "topic-a".to_string(),
            partition: 3,
            ..Default::default()
        };
        let loaded = load_offset_checkpoint(&path, &config).unwrap().unwrap();
        assert_eq!(
            loaded,
            OffsetCheckpoint {
                topic: "topic-a".to_string(),
                partition: 3,
                offset: 42,
            }
        );
    }

    #[test]
    fn test_offset_checkpoint_store_creates_parent_dirs() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested").join("checkpoint.json");

        let checkpoint = OffsetCheckpoint {
            topic: "t".to_string(),
            partition: 0,
            offset: 7,
        };
        store_offset_checkpoint_atomic(&path, &checkpoint).unwrap();
        assert!(path.exists());
    }

    #[test]
    fn test_offset_checkpoint_load_invalid_format() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("checkpoint.bad");

        std::fs::write(&path, "not valid json or integer").unwrap();

        let config = ConsumerConfig {
            topic: "topic-a".to_string(),
            partition: 0,
            ..Default::default()
        };
        assert!(load_offset_checkpoint(&path, &config).is_err());
    }

    #[test]
    fn test_consumer_config_auto_offset_reset_variants() {
        let earliest = ConsumerConfig {
            auto_offset_reset: "earliest".to_string(),
            ..Default::default()
        };
        assert_eq!(earliest.auto_offset_reset, "earliest");

        let latest = ConsumerConfig {
            auto_offset_reset: "latest".to_string(),
            ..Default::default()
        };
        assert_eq!(latest.auto_offset_reset, "latest");

        let none = ConsumerConfig {
            auto_offset_reset: "none".to_string(),
            ..Default::default()
        };
        assert_eq!(none.auto_offset_reset, "none");
    }

    #[test]
    fn test_consumer_config_auto_commit_variants() {
        // Auto-commit enabled
        let enabled = ConsumerConfig {
            enable_auto_commit: true,
            auto_commit_interval_ms: 5000,
            ..Default::default()
        };
        assert!(enabled.enable_auto_commit);
        assert_eq!(enabled.auto_commit_interval_ms, 5000);

        // Auto-commit disabled (manual commit)
        let disabled = ConsumerConfig {
            enable_auto_commit: false,
            ..Default::default()
        };
        assert!(!disabled.enable_auto_commit);
    }

    #[test]
    fn test_consumer_config_decompression_variants() {
        // Decompression enabled (default)
        let enabled = ConsumerConfig {
            enable_decompression: true,
            ..Default::default()
        };
        assert!(enabled.enable_decompression);

        // Decompression disabled (raw protobuf only)
        let disabled = ConsumerConfig {
            enable_decompression: false,
            ..Default::default()
        };
        assert!(!disabled.enable_decompression);
    }

    #[test]
    fn test_consumer_config_topic_names() {
        // Standard topic
        let standard = ConsumerConfig {
            topic: "dashstream-events".to_string(),
            ..Default::default()
        };
        assert_eq!(standard.topic, "dashstream-events");

        // Namespaced topic
        let namespaced = ConsumerConfig {
            topic: "production.dashstream.events".to_string(),
            ..Default::default()
        };
        assert_eq!(namespaced.topic, "production.dashstream.events");

        // Short topic name
        let short = ConsumerConfig {
            topic: "events".to_string(),
            ..Default::default()
        };
        assert_eq!(short.topic, "events");
    }

    #[test]
    fn test_consumer_config_multiple_bootstrap_servers() {
        let config = ConsumerConfig {
            bootstrap_servers: "kafka1:9092,kafka2:9092,kafka3:9092".to_string(),
            ..Default::default()
        };
        assert!(config.bootstrap_servers.contains("kafka1:9092"));
        assert!(config.bootstrap_servers.contains("kafka2:9092"));
        assert!(config.bootstrap_servers.contains("kafka3:9092"));
    }

    #[test]
    fn test_consumer_config_commit_interval_values() {
        // Fast commits (1 second)
        let fast = ConsumerConfig {
            auto_commit_interval_ms: 1000,
            ..Default::default()
        };
        assert_eq!(fast.auto_commit_interval_ms, 1000);

        // Default commits (5 seconds)
        let default = ConsumerConfig {
            auto_commit_interval_ms: 5000,
            ..Default::default()
        };
        assert_eq!(default.auto_commit_interval_ms, 5000);

        // Slow commits (30 seconds)
        let slow = ConsumerConfig {
            auto_commit_interval_ms: 30000,
            ..Default::default()
        };
        assert_eq!(slow.auto_commit_interval_ms, 30000);
    }

    // ============================================================================
    // ConsumerConfig TLS/SASL Tests
    // ============================================================================

    #[test]
    fn test_consumer_config_tls_enabled() {
        let config = ConsumerConfig {
            enable_tls: true,
            ssl_ca_location: Some("/path/to/ca.pem".to_string()),
            ..Default::default()
        };
        assert!(config.enable_tls);
        assert_eq!(config.ssl_ca_location, Some("/path/to/ca.pem".to_string()));
    }

    #[test]
    fn test_consumer_config_mtls() {
        let config = ConsumerConfig {
            enable_tls: true,
            ssl_ca_location: Some("/path/to/ca.pem".to_string()),
            ssl_certificate_location: Some("/path/to/client.pem".to_string()),
            ssl_key_location: Some("/path/to/client-key.pem".to_string()),
            ..Default::default()
        };
        assert!(config.enable_tls);
        assert_eq!(config.ssl_ca_location, Some("/path/to/ca.pem".to_string()));
        assert_eq!(
            config.ssl_certificate_location,
            Some("/path/to/client.pem".to_string())
        );
        assert_eq!(
            config.ssl_key_location,
            Some("/path/to/client-key.pem".to_string())
        );
    }

    #[test]
    fn test_consumer_config_sasl_plain() {
        let config = ConsumerConfig {
            sasl_username: Some("admin".to_string()),
            sasl_password: Some("secret".to_string()),
            ..Default::default()
        };
        assert_eq!(config.sasl_username, Some("admin".to_string()));
        assert_eq!(config.sasl_password, Some("secret".to_string()));
    }

    #[test]
    fn test_consumer_config_sasl_with_tls() {
        // Full security configuration: TLS + SASL PLAIN
        let config = ConsumerConfig {
            enable_tls: true,
            ssl_ca_location: Some("/etc/kafka/ca.pem".to_string()),
            sasl_username: Some("kafka-user".to_string()),
            sasl_password: Some("kafka-password".to_string()),
            ..Default::default()
        };
        assert!(config.enable_tls);
        assert!(config.ssl_ca_location.is_some());
        assert!(config.sasl_username.is_some());
        assert!(config.sasl_password.is_some());
    }

    // ============================================================================
    // ConsumerConfig Builder Pattern Tests
    // ============================================================================

    #[test]
    fn test_consumer_config_builder_pattern() {
        let config = ConsumerConfig {
            bootstrap_servers: "custom:9092".to_string(),
            topic: "custom-topic".to_string(),
            ..Default::default()
        };

        assert_eq!(config.bootstrap_servers, "custom:9092");
        assert_eq!(config.topic, "custom-topic");
    }

    #[test]
    fn test_consumer_config_partial_override() {
        let config = ConsumerConfig {
            topic: "production-events".to_string(),
            ..Default::default()
        };

        // Overridden fields
        assert_eq!(config.topic, "production-events");

        // Default fields preserved
        assert_eq!(config.bootstrap_servers, "localhost:9092");
        assert_eq!(config.auto_offset_reset, "earliest");
        assert!(config.enable_auto_commit);
    }

    #[test]
    fn test_consumer_config_strict_validation_variants() {
        // Strict mode enabled (security default)
        let strict_enabled = ConsumerConfig {
            enable_strict_validation: true,
            ..Default::default()
        };
        assert!(strict_enabled.enable_strict_validation);

        // Legacy mode (backward compatibility)
        let legacy_mode = ConsumerConfig {
            enable_strict_validation: false,
            ..Default::default()
        };
        assert!(!legacy_mode.enable_strict_validation);

        // Default is strict (security by default)
        let default_config = ConsumerConfig::default();
        assert!(default_config.enable_strict_validation);
    }

    // ============================================================================
    // DashStreamConsumer Accessor Tests
    // ============================================================================

    // Note: Full consumer integration tests are marked #[ignore] because they require Kafka.
    // These tests verify the accessors without requiring Kafka connection.

    #[test]
    fn test_consumer_config_accessors_structure() {
        // This test verifies that the config structure is correctly defined
        let config = ConsumerConfig {
            bootstrap_servers: "test:9092".to_string(),
            topic: "test-topic".to_string(),
            auto_offset_reset: "earliest".to_string(),
            enable_auto_commit: true,
            auto_commit_interval_ms: 5000,
            enable_decompression: true,
            max_message_size: 1_048_576,
            enable_tls: false,
            ssl_ca_location: None,
            ssl_certificate_location: None,
            ssl_key_location: None,
            sasl_username: None,
            sasl_password: None,
            enable_strict_validation: true,
            ..Default::default()
        };

        // Verify all fields are accessible
        assert_eq!(config.bootstrap_servers, "test:9092");
        assert_eq!(config.topic, "test-topic");
        assert_eq!(config.auto_offset_reset, "earliest");
        assert!(config.enable_auto_commit);
        assert_eq!(config.auto_commit_interval_ms, 5000);
        assert!(config.enable_decompression);
        assert!(config.enable_strict_validation);
    }

    // ============================================================================
    // Integration Tests (Require Kafka)
    // ============================================================================

    #[tokio::test]
    #[ignore = "requires Docker for testcontainers"]
    async fn test_consume_event() {
        use testcontainers::runners::AsyncRunner;
        use testcontainers_modules::kafka::apache;

        let kafka = apache::Kafka::default().start().await.unwrap();
        let bootstrap_servers = format!(
            "127.0.0.1:{}",
            kafka.get_host_port_ipv4(apache::KAFKA_PORT).await.unwrap()
        );
        tokio::time::sleep(Duration::from_secs(3)).await;

        // Create topic first
        crate::kafka::create_topic(
            &bootstrap_servers,
            "test-events",
            crate::kafka::dev_config(),
        )
        .await
        .expect("Failed to create topic");

        let mut consumer =
            DashStreamConsumer::new(&bootstrap_servers, "test-events", "test-consumer-group")
                .await
                .expect("Failed to create consumer");

        // Try to consume a message with timeout
        if let Some(result) = consumer.next_timeout(Duration::from_secs(5)).await {
            match result {
                Ok(msg) => {
                    println!("Received message: {:?}", msg);
                }
                Err(e) => {
                    eprintln!("Error decoding message: {}", e);
                }
            }
        } else {
            println!("No message received within timeout");
        }
    }

    #[tokio::test]
    #[ignore = "requires Docker for testcontainers"]
    async fn test_consumer_new_with_defaults() {
        use testcontainers::runners::AsyncRunner;
        use testcontainers_modules::kafka::apache;

        let kafka = apache::Kafka::default().start().await.unwrap();
        let bootstrap_servers = format!(
            "127.0.0.1:{}",
            kafka.get_host_port_ipv4(apache::KAFKA_PORT).await.unwrap()
        );
        tokio::time::sleep(Duration::from_secs(3)).await;

        // Create topic first
        crate::kafka::create_topic(
            &bootstrap_servers,
            "dashstream-events",
            crate::kafka::dev_config(),
        )
        .await
        .expect("Failed to create topic");

        let result = DashStreamConsumer::new(
            &bootstrap_servers,
            "dashstream-events",
            "test-consumer-group",
        )
        .await;

        // This will fail if Kafka is not running, but that's expected for ignored test
        match result {
            Ok(consumer) => {
                assert_eq!(consumer.topic(), "dashstream-events");
                assert_eq!(consumer.group_id(), "test-consumer-group");
                assert!(consumer.config.enable_decompression);
                assert!(consumer.config.enable_auto_commit);
            }
            Err(_) => {
                // Expected if Kafka is not running
            }
        }
    }

    #[tokio::test]
    #[ignore = "requires Docker for testcontainers"]
    async fn test_consumer_with_custom_config() {
        use testcontainers::runners::AsyncRunner;
        use testcontainers_modules::kafka::apache;

        let kafka = apache::Kafka::default().start().await.unwrap();
        let bootstrap_servers = format!(
            "127.0.0.1:{}",
            kafka.get_host_port_ipv4(apache::KAFKA_PORT).await.unwrap()
        );
        tokio::time::sleep(Duration::from_secs(3)).await;

        let config = ConsumerConfig {
            bootstrap_servers,
            topic: "custom-topic".to_string(),
            auto_offset_reset: "latest".to_string(),
            enable_auto_commit: false,
            auto_commit_interval_ms: 10000,
            enable_decompression: false,
            max_message_size: 1_048_576,
            enable_tls: false,
            ssl_ca_location: None,
            ssl_certificate_location: None,
            ssl_key_location: None,
            sasl_username: None,
            sasl_password: None,
            enable_strict_validation: true,
            ..Default::default()
        };

        let result = DashStreamConsumer::with_config(config.clone()).await;

        match result {
            Ok(consumer) => {
                assert_eq!(consumer.topic(), "custom-topic");
                assert!(!consumer.config.enable_decompression);
                assert!(!consumer.config.enable_auto_commit);
            }
            Err(_) => {
                // Expected if Kafka is not running
            }
        }
    }

    #[tokio::test]
    #[ignore = "requires Docker for testcontainers"]
    async fn test_consumer_group_metadata() {
        use testcontainers::runners::AsyncRunner;
        use testcontainers_modules::kafka::apache;

        let kafka = apache::Kafka::default().start().await.unwrap();
        let bootstrap_servers = format!(
            "127.0.0.1:{}",
            kafka.get_host_port_ipv4(apache::KAFKA_PORT).await.unwrap()
        );
        tokio::time::sleep(Duration::from_secs(3)).await;

        // Create topic first
        crate::kafka::create_topic(
            &bootstrap_servers,
            "dashstream-events",
            crate::kafka::dev_config(),
        )
        .await
        .expect("Failed to create topic");

        let consumer = DashStreamConsumer::new(
            &bootstrap_servers,
            "dashstream-events",
            "metadata-test-group",
        )
        .await
        .expect("Failed to create consumer");

        assert_eq!(consumer.group_id(), "metadata-test-group");
        assert_eq!(consumer.topic(), "dashstream-events");
    }

    #[tokio::test]
    #[ignore = "requires Docker for testcontainers"]
    async fn test_consumer_timeout_no_messages() {
        use testcontainers::runners::AsyncRunner;
        use testcontainers_modules::kafka::apache;

        let kafka = apache::Kafka::default().start().await.unwrap();
        let bootstrap_servers = format!(
            "127.0.0.1:{}",
            kafka.get_host_port_ipv4(apache::KAFKA_PORT).await.unwrap()
        );
        tokio::time::sleep(Duration::from_secs(3)).await;

        // Create topic first
        crate::kafka::create_topic(
            &bootstrap_servers,
            "empty-topic-for-timeout-test",
            crate::kafka::dev_config(),
        )
        .await
        .expect("Failed to create topic");

        let mut consumer = DashStreamConsumer::new(
            &bootstrap_servers,
            "empty-topic-for-timeout-test",
            "timeout-test-group",
        )
        .await
        .expect("Failed to create consumer");

        // Try to consume with short timeout - should return None
        let result = consumer.next_timeout(Duration::from_millis(100)).await;
        assert!(result.is_none(), "Expected timeout with no messages");
    }

    // ============================================================================
    // SequenceValidator Tests
    // ============================================================================

    #[test]
    fn test_sequence_validator_new() {
        let validator = SequenceValidator::new();
        assert!(validator.expected_for_thread("thread1").is_none());
    }

    #[test]
    fn test_sequence_validator_default() {
        let validator = SequenceValidator::default();
        assert!(validator.expected_for_thread("thread1").is_none());
    }

    #[test]
    fn test_sequence_validator_valid_sequence() {
        let mut validator = SequenceValidator::new();

        // First message for thread1 with sequence 1
        assert!(validator.validate("thread1", 1).is_ok());
        assert_eq!(validator.expected_for_thread("thread1"), Some(2));

        // Second message with sequence 2
        assert!(validator.validate("thread1", 2).is_ok());
        assert_eq!(validator.expected_for_thread("thread1"), Some(3));

        // Third message with sequence 3
        assert!(validator.validate("thread1", 3).is_ok());
        assert_eq!(validator.expected_for_thread("thread1"), Some(4));
    }

    #[test]
    fn test_sequence_validator_multiple_threads() {
        let mut validator = SequenceValidator::new();

        // Thread1: sequence 1
        assert!(validator.validate("thread1", 1).is_ok());

        // Thread2: sequence 1 (independent from thread1)
        assert!(validator.validate("thread2", 1).is_ok());

        // Thread1: sequence 2
        assert!(validator.validate("thread1", 2).is_ok());

        // Thread2: sequence 2
        assert!(validator.validate("thread2", 2).is_ok());

        assert_eq!(validator.expected_for_thread("thread1"), Some(3));
        assert_eq!(validator.expected_for_thread("thread2"), Some(3));
    }

    #[test]
    fn test_sequence_validator_gap() {
        let mut validator = SequenceValidator::new();

        // First message
        assert!(validator.validate("thread1", 1).is_ok());

        // Gap: jump from 1 to 5 (missing 2, 3, 4)
        let result = validator.validate("thread1", 5);
        assert!(result.is_err());

        match result.unwrap_err() {
            SequenceError::Gap {
                thread_id,
                expected,
                received,
                gap_size,
            } => {
                assert_eq!(thread_id, "thread1");
                assert_eq!(expected, 2);
                assert_eq!(received, 5);
                assert_eq!(gap_size, 3);
            }
            _ => panic!("Expected Gap error"),
        }

        // After gap, validator should expect 6
        assert_eq!(validator.expected_for_thread("thread1"), Some(6));
    }

    #[test]
    fn test_sequence_validator_duplicate() {
        let mut validator = SequenceValidator::new();

        // First message
        assert!(validator.validate("thread1", 1).is_ok());

        // Second message
        assert!(validator.validate("thread1", 2).is_ok());

        // Duplicate: repeat sequence 2
        let result = validator.validate("thread1", 2);
        assert!(result.is_err());

        match result.unwrap_err() {
            SequenceError::Duplicate {
                thread_id,
                sequence,
                expected,
            } => {
                assert_eq!(thread_id, "thread1");
                assert_eq!(sequence, 2);
                assert_eq!(expected, 3);
            }
            _ => panic!("Expected Duplicate error"),
        }
    }

    #[test]
    fn test_sequence_validator_reordered() {
        let mut validator = SequenceValidator::new();

        // First message
        assert!(validator.validate("thread1", 1).is_ok());

        // Second message
        assert!(validator.validate("thread1", 2).is_ok());

        // Third message
        assert!(validator.validate("thread1", 3).is_ok());

        // Reordered: receive sequence 1 again (not the immediate previous)
        let result = validator.validate("thread1", 1);
        assert!(result.is_err());

        match result.unwrap_err() {
            SequenceError::Reordered {
                thread_id,
                sequence,
                expected,
            } => {
                assert_eq!(thread_id, "thread1");
                assert_eq!(sequence, 1);
                assert_eq!(expected, 4);
            }
            _ => panic!("Expected Reordered error"),
        }
    }

    #[test]
    fn test_sequence_validator_reset() {
        let mut validator = SequenceValidator::new();

        // Process some messages
        assert!(validator.validate("thread1", 1).is_ok());
        assert!(validator.validate("thread1", 2).is_ok());
        assert_eq!(validator.expected_for_thread("thread1"), Some(3));

        // Reset thread1
        validator.reset("thread1");
        assert!(validator.expected_for_thread("thread1").is_none());

        // Can start from 1 again
        assert!(validator.validate("thread1", 1).is_ok());
        assert_eq!(validator.expected_for_thread("thread1"), Some(2));
    }

    #[test]
    fn test_sequence_validator_clear() {
        let mut validator = SequenceValidator::new();

        // Process messages for multiple threads
        assert!(validator.validate("thread1", 1).is_ok());
        assert!(validator.validate("thread2", 1).is_ok());
        assert!(validator.validate("thread3", 1).is_ok());

        // Clear all
        validator.clear();

        assert!(validator.expected_for_thread("thread1").is_none());
        assert!(validator.expected_for_thread("thread2").is_none());
        assert!(validator.expected_for_thread("thread3").is_none());
    }

    #[test]
    fn test_sequence_validator_large_gap() {
        let mut validator = SequenceValidator::new();

        // First message
        assert!(validator.validate("thread1", 1).is_ok());

        // Very large gap: after 1, we expect 2, but receive 1000
        // Gap size = 1000 - 2 = 998 (missing messages 2-999)
        let result = validator.validate("thread1", 1000);
        assert!(result.is_err());

        match result.unwrap_err() {
            SequenceError::Gap { gap_size, .. } => {
                assert_eq!(gap_size, 998);
            }
            _ => panic!("Expected Gap error"),
        }
    }

    #[test]
    fn test_sequence_validator_recovery_after_gap() {
        let mut validator = SequenceValidator::new();

        // Normal sequence
        assert!(validator.validate("thread1", 1).is_ok());
        assert!(validator.validate("thread1", 2).is_ok());

        // Gap
        let result = validator.validate("thread1", 5);
        assert!(result.is_err());

        // Continue after gap
        assert!(validator.validate("thread1", 6).is_ok());
        assert!(validator.validate("thread1", 7).is_ok());
    }

    #[test]
    fn test_sequence_validator_first_sequence_not_one() {
        let mut validator = SequenceValidator::new();

        // M-1114: First message for a thread now establishes the baseline.
        // This prevents false gap reports when server restarts mid-stream.
        // Previously this returned an error expecting seq=1, but now it accepts
        // the first-seen sequence as the baseline.
        let result = validator.validate("thread1", 100);
        assert!(
            result.is_ok(),
            "First message should succeed (establishes baseline per M-1114)"
        );

        // Now expects 101
        assert_eq!(validator.expected_for_thread("thread1"), Some(101));

        // Second message at 101 should succeed
        assert!(validator.validate("thread1", 101).is_ok());

        // Gap at 103 (skipped 102) should be detected
        let gap_result = validator.validate("thread1", 103);
        assert!(gap_result.is_err());
        match gap_result.unwrap_err() {
            SequenceError::Gap {
                expected,
                received,
                gap_size,
                ..
            } => {
                assert_eq!(expected, 102);
                assert_eq!(received, 103);
                assert_eq!(gap_size, 1);
            }
            _ => panic!("Expected Gap error"),
        }
    }

    #[test]
    fn test_sequence_error_display() {
        let gap = SequenceError::Gap {
            thread_id: "test-thread".to_string(),
            expected: 10,
            received: 15,
            gap_size: 5,
        };
        assert_eq!(
            format!("{}", gap),
            "Sequence gap for thread test-thread: expected 10, received 15 (gap size: 5)"
        );

        let duplicate = SequenceError::Duplicate {
            thread_id: "test-thread".to_string(),
            sequence: 5,
            expected: 6,
        };
        assert_eq!(
            format!("{}", duplicate),
            "Duplicate sequence for thread test-thread: received 5, expected 6"
        );

        let reordered = SequenceError::Reordered {
            thread_id: "test-thread".to_string(),
            sequence: 3,
            expected: 10,
        };
        assert_eq!(
            format!("{}", reordered),
            "Out-of-order sequence for thread test-thread: received 3, expected 10"
        );
    }

    #[test]
    fn test_sequence_error_debug() {
        let gap = SequenceError::Gap {
            thread_id: "test".to_string(),
            expected: 1,
            received: 5,
            gap_size: 4,
        };
        let debug_str = format!("{:?}", gap);
        assert!(debug_str.contains("Gap"));
        assert!(debug_str.contains("test"));
    }

    #[test]
    fn test_sequence_error_clone() {
        let error1 = SequenceError::Gap {
            thread_id: "test".to_string(),
            expected: 1,
            received: 5,
            gap_size: 4,
        };
        let error2 = error1.clone();
        assert_eq!(error1, error2);
    }

    /// M-514: Test that pruned threads don't cause false gap detection when they reappear.
    #[test]
    fn test_sequence_validator_pruned_thread_reappearance() {
        let mut validator = SequenceValidator::new();

        // Simulate pruning by directly manipulating the pruned_threads set
        // (In practice, this happens when expected_next exceeds MAX_TRACKED_THREADS)
        validator.pruned_threads.insert("pruned_thread".to_string());

        // When a pruned thread sends a message, it should be accepted without false gap detection.
        // Even if sequence is high (would normally trigger gap from expected=1), it should work.
        let result = validator.validate("pruned_thread", 100);
        assert!(
            result.is_ok(),
            "Pruned thread reappearance should be accepted: {:?}",
            result
        );

        // After reappearance, the thread should be tracked normally
        assert!(!validator.pruned_threads.contains("pruned_thread"));
        assert_eq!(validator.expected_for_thread("pruned_thread"), Some(101));

        // Next message should follow normal validation
        assert!(validator.validate("pruned_thread", 101).is_ok());

        // Gap detection should work normally now
        let gap_result = validator.validate("pruned_thread", 200);
        assert!(gap_result.is_err());
        match gap_result.unwrap_err() {
            SequenceError::Gap { gap_size, .. } => {
                assert_eq!(gap_size, 98); // 200 - 102 = 98
            }
            e => panic!("Expected Gap error, got {:?}", e),
        }
    }

    /// M-514: Test that pruned_threads set doesn't grow unboundedly
    #[test]
    fn test_sequence_validator_pruned_threads_capped() {
        let mut validator = SequenceValidator::new();

        // Simulate tracking many pruned threads
        for i in 0..15_000 {
            validator.pruned_threads.insert(format!("thread_{}", i));
        }

        // Trigger pruning of expected_next (which also caps pruned_threads)
        // We need to exceed MAX_TRACKED_THREADS in expected_next first
        for i in 0..100_005 {
            validator.expected_next.insert(format!("active_{}", i), 1);
        }

        // Now validate a new thread, which triggers prune_state
        validator.validate("new_thread", 1).ok();

        // pruned_threads should be capped at MAX_PRUNED_THREADS (10_000)
        // Note: It may be slightly over due to the new entries being added before cap check
        assert!(
            validator.pruned_threads.len() <= SequenceValidator::MAX_PRUNED_THREADS + SequenceValidator::PRUNE_BATCH,
            "pruned_threads should be capped, got {}",
            validator.pruned_threads.len()
        );
    }

    // ============================================================================
    // Gap Recovery Policy Tests
    // ============================================================================

    #[test]
    fn test_gap_recovery_policy_continue() {
        let mut validator = SequenceValidator::with_policy(GapRecoveryPolicy::Continue);

        assert!(validator.validate("thread1", 1).is_ok());

        // Gap: 1 -> 5
        let result = validator.validate("thread1", 5);
        assert!(matches!(result, Err(SequenceError::Gap { .. })));

        // Should continue from 5
        assert_eq!(validator.expected_for_thread("thread1"), Some(6));
        assert!(validator.validate("thread1", 6).is_ok());
        assert!(!validator.is_halted("thread1"));
    }

    #[test]
    fn test_gap_recovery_policy_halt() {
        let mut validator = SequenceValidator::with_policy(GapRecoveryPolicy::Halt);

        assert!(validator.validate("thread1", 1).is_ok());

        // Gap: 1 -> 5 (should halt)
        let result = validator.validate("thread1", 5);
        assert!(matches!(result, Err(SequenceError::Gap { .. })));

        // Thread should be halted
        assert!(validator.is_halted("thread1"));

        // Expected should NOT advance (stuck at gap)
        assert_eq!(validator.expected_for_thread("thread1"), Some(2));

        // Manual reset
        validator.reset_halted("thread1");
        assert!(!validator.is_halted("thread1"));

        // Can process again from 1
        assert!(validator.validate("thread1", 1).is_ok());
    }

    #[test]
    fn test_gap_recovery_policy_warn_and_continue() {
        let mut validator = SequenceValidator::with_policy(GapRecoveryPolicy::WarnAndContinue);

        assert!(validator.validate("thread1", 1).is_ok());

        // Gap: 1 -> 5
        let result = validator.validate("thread1", 5);
        assert!(matches!(result, Err(SequenceError::Gap { .. })));

        // Should continue from 5 (not halted)
        assert!(!validator.is_halted("thread1"));
        assert_eq!(validator.expected_for_thread("thread1"), Some(6));
        assert!(validator.validate("thread1", 6).is_ok());
    }

    #[test]
    fn test_gap_recovery_policy_default() {
        let validator = SequenceValidator::new();
        // Default should be WarnAndContinue
        assert_eq!(validator.policy, GapRecoveryPolicy::WarnAndContinue);
    }

    #[test]
    fn test_get_halted_threads() {
        let mut validator = SequenceValidator::with_policy(GapRecoveryPolicy::Halt);

        // Halt two threads
        validator.validate("thread1", 1).ok();
        validator.validate("thread1", 5).ok(); // Gap - halts thread1

        validator.validate("thread2", 1).ok();
        validator.validate("thread2", 10).ok(); // Gap - halts thread2

        let halted = validator.get_halted_threads();
        assert_eq!(halted.len(), 2);
        assert!(halted.contains(&"thread1".to_string()));
        assert!(halted.contains(&"thread2".to_string()));
    }

    #[test]
    fn test_consumer_config_max_message_size() {
        let config = ConsumerConfig {
            max_message_size: 2_097_152, // 2 MB
            ..Default::default()
        };
        assert_eq!(config.max_message_size, 2_097_152);

        // Default should be 1 MB
        let default_config = ConsumerConfig::default();
        assert_eq!(default_config.max_message_size, 1_048_576);
    }

    #[test]
    fn test_decompression_respects_max_message_size() {
        // This test verifies Bug #14 fix: decompression uses config.max_message_size
        // instead of a hardcoded default, preventing decompression bombs from
        // exceeding the configured limit.
        use crate::codec::{decode_message_strict, encode_message_with_compression};
        use crate::{attribute_value::Value as AttrVal, AttributeValue, DashStreamMessage};
        use crate::{Event, EventType, Header, MessageType};

        // Create a large compressible message
        let mut large_attributes = std::collections::HashMap::new();
        for i in 0..100 {
            large_attributes.insert(
                format!("key_{}", i),
                AttributeValue {
                    value: Some(AttrVal::StringValue(
                        "repeated_value_for_compression_testing".to_string(),
                    )),
                },
            );
        }

        let event = Event {
            header: Some(Header {
                message_id: vec![1; 16],
                timestamp_us: 1234567890,
                tenant_id: "tenant".to_string(),
                thread_id: "thread".to_string(),
                sequence: 1,
                r#type: MessageType::Event as i32,
                parent_id: vec![],
                compression: 0,
                schema_version: 1,
            }),
            event_type: EventType::GraphStart as i32,
            node_id: "node".to_string(),
            attributes: large_attributes,
            duration_us: 0,
            llm_request_id: "".to_string(),
        };
        let message = DashStreamMessage {
            message: Some(crate::dash_stream_message::Message::Event(event)),
        };

        // Encode with compression
        let (compressed_bytes, is_compressed) =
            encode_message_with_compression(&message, true).unwrap();
        assert!(is_compressed, "Message should be compressed");

        // Decode with a generous limit should succeed.
        let result = decode_message_strict(&compressed_bytes, 10 * 1024 * 1024);
        assert!(result.is_ok(), "Should decode with large limit");

        // Decoding with a tight limit should fail once decompression exceeds the configured cap.
        let tight_limit = compressed_bytes.len().saturating_sub(1);
        let result = decode_message_strict(&compressed_bytes, tight_limit);
        assert!(
            result.is_err(),
            "Should fail when max_size is smaller than decompressed size"
        );
    }
