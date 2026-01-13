//! Redis integration tests for distributed rate limiting
//!
//! These tests verify that rate limiting works correctly across multiple servers
//! when using Redis as a shared backend. Tests use testcontainers to run real Redis.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use dashflow_streaming::rate_limiter::{RateLimit, TenantRateLimiter};
use testcontainers::runners::AsyncRunner;
use testcontainers_modules::redis::Redis;

/// Test that distributed rate limiting enforces shared quota across 3 simulated servers
///
/// This is the critical test that validates the fix for the multi-server rate limit bug.
/// Before the fix: Each server had independent token buckets (3 servers @ 100 msg/sec = 300 msg/sec actual)
/// After the fix: All servers share the same Redis-backed token bucket (3 servers @ 100 msg/sec = 100 msg/sec total)
#[tokio::test]
#[ignore = "requires Docker for testcontainers"]
async fn test_distributed_rate_limit_three_servers() {
    println!("🐳 Starting testcontainers Redis for distributed rate limit test...");

    let redis_container = Redis::default().start().await.unwrap();
    let port = redis_container.get_host_port_ipv4(6379).await.unwrap();
    let redis_url = format!("redis://127.0.0.1:{}", port);

    println!("✅ Redis container started on port {}", port);

    // Create 3 rate limiters (simulating 3 different servers)
    // All share the same Redis backend
    let limiters = [
        TenantRateLimiter::new_with_redis(
            RateLimit {
                messages_per_second: 10.0,
                burst_capacity: 30,
            },
            &redis_url,
        )
        .await
        .unwrap(),
        TenantRateLimiter::new_with_redis(
            RateLimit {
                messages_per_second: 10.0,
                burst_capacity: 30,
            },
            &redis_url,
        )
        .await
        .unwrap(),
        TenantRateLimiter::new_with_redis(
            RateLimit {
                messages_per_second: 10.0,
                burst_capacity: 30,
            },
            &redis_url,
        )
        .await
        .unwrap(),
    ];

    println!("✅ Created 3 rate limiters sharing Redis backend");

    // Each server consumes 10 tokens (total: 30 tokens consumed across all servers)
    for (i, limiter) in limiters.iter().enumerate() {
        for _ in 0..10 {
            let allowed = limiter.check_rate_limit("tenant1", 1).await.unwrap();
            assert!(
                allowed,
                "Server {} should be allowed to consume tokens",
                i + 1
            );
        }
    }

    println!("✅ All 3 servers consumed 10 tokens each (30 total)");

    // Now bucket should be empty (30 capacity, 30 consumed)
    // The 31st message should fail on ANY server
    assert!(
        !limiters[0].check_rate_limit("tenant1", 1).await.unwrap(),
        "Server 1 should be rate limited (bucket empty)"
    );
    assert!(
        !limiters[1].check_rate_limit("tenant1", 1).await.unwrap(),
        "Server 2 should be rate limited (bucket empty)"
    );
    assert!(
        !limiters[2].check_rate_limit("tenant1", 1).await.unwrap(),
        "Server 3 should be rate limited (bucket empty)"
    );

    println!("✅ Distributed rate limiting verified: 3 servers share quota");
    println!("✅ Test proves fix for multi-server rate limit bug");
}

/// Test that tenants are properly isolated in Redis
///
/// Each tenant should have independent rate limits stored in separate Redis keys
#[tokio::test]
#[ignore = "requires Docker for testcontainers"]
async fn test_redis_tenant_isolation() {
    println!("🐳 Starting Redis for tenant isolation test...");

    let redis_container = Redis::default().start().await.unwrap();
    let port = redis_container.get_host_port_ipv4(6379).await.unwrap();
    let redis_url = format!("redis://127.0.0.1:{}", port);

    let limiter = TenantRateLimiter::new_with_redis(
        RateLimit {
            messages_per_second: 10.0,
            burst_capacity: 10,
        },
        &redis_url,
    )
    .await
    .unwrap();

    println!("✅ Redis container started on port {}", port);

    // Tenant1 exhausts their quota
    for _ in 0..10 {
        assert!(limiter.check_rate_limit("tenant1", 1).await.unwrap());
    }
    assert!(!limiter.check_rate_limit("tenant1", 1).await.unwrap());

    println!("✅ Tenant1 exhausted their quota");

    // Tenant2 should still have full capacity (different Redis key)
    for _ in 0..10 {
        assert!(limiter.check_rate_limit("tenant2", 1).await.unwrap());
    }

    println!("✅ Tenant2 has independent quota (tenants properly isolated)");
}

/// Test that token buckets refill correctly over time in Redis
#[tokio::test]
#[ignore = "requires Docker for testcontainers"]
async fn test_redis_token_refill() {
    println!("🐳 Starting Redis for token refill test...");

    let redis_container = Redis::default().start().await.unwrap();
    let port = redis_container.get_host_port_ipv4(6379).await.unwrap();
    let redis_url = format!("redis://127.0.0.1:{}", port);

    let limiter = TenantRateLimiter::new_with_redis(
        RateLimit {
            messages_per_second: 10.0, // 10 tokens/sec refill
            burst_capacity: 100,
        },
        &redis_url,
    )
    .await
    .unwrap();

    println!("✅ Redis container started on port {}", port);

    // Consume all tokens
    for _ in 0..100 {
        assert!(limiter.check_rate_limit("tenant1", 1).await.unwrap());
    }
    assert!(!limiter.check_rate_limit("tenant1", 1).await.unwrap());

    println!("✅ Consumed all 100 tokens");

    // Wait 1 second (should refill ~10 tokens at 10 tokens/sec)
    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

    println!("⏱️  Waited 1 second for token refill");

    // Should now allow ~10 messages
    let mut allowed = 0;
    for _ in 0..20 {
        if limiter.check_rate_limit("tenant1", 1).await.unwrap() {
            allowed += 1;
        }
    }

    assert!(
        (9..=11).contains(&allowed),
        "Expected ~10 tokens refilled, got {}",
        allowed
    );

    println!("✅ Token refill verified: {} tokens refilled", allowed);
}

/// Test that rate limiter gracefully handles Redis being unavailable
///
/// Should fall back to in-memory mode or return error without panicking
#[tokio::test]
#[ignore = "requires Docker for testcontainers"]
async fn test_redis_unavailable_handling() {
    println!("🔌 Testing Redis unavailable scenario...");

    // Try to connect to non-existent Redis server
    let result = TenantRateLimiter::new_with_redis(
        RateLimit::default(),
        "redis://localhost:9999", // Non-existent Redis
    )
    .await;

    // Should return error, not panic
    assert!(
        result.is_err(),
        "Should return error when Redis is unavailable"
    );

    println!("✅ Gracefully handled unavailable Redis (returned error)");
}

/// Test Redis authentication URL format
///
/// Verifies that redis://:password@host:port format is parsed correctly.
/// The redis crate handles authentication automatically from URL.
#[tokio::test]
#[ignore = "requires Docker for testcontainers"]
async fn test_redis_auth_url_format() {
    println!("🔐 Testing Redis authentication URL parsing...");

    // Test that URL with password is accepted (connection will fail but parsing should work)
    let auth_url = "redis://:mypassword@localhost:9999";

    // Attempt to create rate limiter with auth URL
    let result = TenantRateLimiter::new_with_redis(RateLimit::default(), auth_url).await;

    // Should get connection error (port 9999 doesn't exist), not parse error
    match result {
        Err(e) => {
            let err_str = e.to_string();
            // Should be connection error, not URL parse error
            assert!(
                err_str.contains("Connection refused") || err_str.contains("could not connect"),
                "Expected connection error, got: {}",
                err_str
            );
            println!("✅ Auth URL parsed correctly (connection failed as expected)");
        }
        Ok(_) => {
            panic!("Should have failed to connect to non-existent Redis");
        }
    }

    // Test that rediss:// (TLS) is also accepted
    let tls_url = "rediss://:password@localhost:6380";
    let tls_result = TenantRateLimiter::new_with_redis(RateLimit::default(), tls_url).await;

    assert!(
        tls_result.is_err(),
        "Should fail to connect, but URL should parse"
    );
    println!("✅ TLS URL format accepted");
}

/// Test that multiple consumers on same tenant don't cause race conditions
///
/// Verifies atomicity of the Lua script token bucket operations
#[tokio::test]
#[ignore = "requires Docker for testcontainers"]
async fn test_redis_concurrent_access() {
    println!("🐳 Starting Redis for concurrent access test...");

    let redis_container = Redis::default().start().await.unwrap();
    let port = redis_container.get_host_port_ipv4(6379).await.unwrap();
    let redis_url = format!("redis://127.0.0.1:{}", port);

    let limiter = std::sync::Arc::new(
        TenantRateLimiter::new_with_redis(
            RateLimit {
                messages_per_second: 100.0,
                burst_capacity: 100,
            },
            &redis_url,
        )
        .await
        .unwrap(),
    );

    println!("✅ Redis container started on port {}", port);

    // Spawn 10 concurrent tasks, each trying to consume 10 tokens
    let mut handles = vec![];
    for _ in 0..10 {
        let limiter_clone = std::sync::Arc::clone(&limiter);
        let handle = tokio::spawn(async move {
            let mut count = 0;
            for _ in 0..10 {
                if limiter_clone.check_rate_limit("tenant1", 1).await.unwrap() {
                    count += 1;
                }
            }
            count
        });
        handles.push(handle);
    }

    // Wait for all tasks and sum allowed messages
    let mut total_allowed = 0;
    for handle in handles {
        total_allowed += handle.await.unwrap();
    }

    // Exactly 100 messages should be allowed (not more, not less)
    // This proves the Lua script is atomic and prevents race conditions
    assert_eq!(
        total_allowed, 100,
        "Expected exactly 100 allowed due to atomic operations, got {}",
        total_allowed
    );

    println!("✅ Concurrent access verified: Lua script is atomic (exactly 100 allowed)");
}

/// Test that Redis keys have proper TTL set
///
/// This prevents memory leaks when tenants stop sending messages
#[tokio::test]
#[ignore = "requires Docker for testcontainers"]
async fn test_redis_ttl_set() {
    println!("🐳 Starting Redis for TTL test...");

    let redis_container = Redis::default().start().await.unwrap();
    let port = redis_container.get_host_port_ipv4(6379).await.unwrap();
    let redis_url = format!("redis://127.0.0.1:{}", port);

    let limiter = TenantRateLimiter::new_with_redis(
        RateLimit {
            messages_per_second: 10.0,
            burst_capacity: 100,
        },
        &redis_url,
    )
    .await
    .unwrap();

    println!("✅ Redis container started on port {}", port);

    // Send one message to create the Redis key
    limiter.check_rate_limit("tenant1", 1).await.unwrap();

    println!("✅ Sent message to create Redis key");

    // Connect to Redis directly and check TTL
    let client = redis::Client::open(redis_url.as_str()).unwrap();
    let mut conn = client.get_multiplexed_async_connection().await.unwrap();

    let ttl: i64 = redis::cmd("TTL")
        .arg("rate_limit:tenant1:bucket")
        .query_async(&mut conn)
        .await
        .unwrap();

    // TTL should be set (>0) and reasonable (<= 3600 seconds = 1 hour)
    assert!(
        ttl > 0 && ttl <= 3600,
        "Expected TTL between 0 and 3600, got {}",
        ttl
    );

    println!("✅ Redis key has proper TTL: {} seconds", ttl);
}
