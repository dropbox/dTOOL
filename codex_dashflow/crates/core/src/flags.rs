//! Feature flags and test fixtures
//!
//! Provides environment variable-based feature flags for testing and development.
//! These flags allow configuring test fixtures and other runtime behavior.

use std::sync::OnceLock;

/// Get the SSE fixture path for offline tests.
///
/// When set via `CODEX_RS_SSE_FIXTURE` environment variable, the SSE client
/// will read responses from this file instead of making real HTTP requests.
/// This is useful for deterministic testing.
///
/// # Returns
/// - `Some(path)` if `CODEX_RS_SSE_FIXTURE` is set
/// - `None` otherwise
pub fn get_sse_fixture() -> Option<&'static str> {
    static SSE_FIXTURE: OnceLock<Option<String>> = OnceLock::new();
    SSE_FIXTURE
        .get_or_init(|| std::env::var("CODEX_RS_SSE_FIXTURE").ok())
        .as_deref()
}

/// Check if SSE fixture mode is enabled for tests.
pub fn is_sse_fixture_enabled() -> bool {
    get_sse_fixture().is_some()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, MutexGuard, OnceLock, PoisonError};

    /// Guard for controlling access to SSE fixture env var during tests.
    /// This ensures tests don't interfere with each other.
    struct EnvGuard {
        _guard: MutexGuard<'static, ()>,
    }

    impl EnvGuard {
        fn new() -> Self {
            static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
            let guard = LOCK
                .get_or_init(Mutex::default)
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            Self { _guard: guard }
        }
    }

    #[test]
    fn test_get_sse_fixture_default() {
        let _guard = EnvGuard::new();
        // The OnceLock will cache the initial value, so we can't test
        // dynamic changes. Just verify it doesn't panic.
        let _ = get_sse_fixture();
    }

    #[test]
    fn test_is_sse_fixture_enabled_consistency() {
        let _guard = EnvGuard::new();
        // Verify that is_sse_fixture_enabled matches get_sse_fixture
        let fixture = get_sse_fixture();
        let enabled = is_sse_fixture_enabled();
        assert_eq!(fixture.is_some(), enabled);
    }

    #[test]
    fn test_get_sse_fixture_is_static() {
        let _guard = EnvGuard::new();
        // Verify that multiple calls return the same reference
        let first = get_sse_fixture();
        let second = get_sse_fixture();
        // Both should be identical (same memory location if Some)
        match (first, second) {
            (Some(a), Some(b)) => assert!(std::ptr::eq(a, b)),
            (None, None) => {}
            _ => panic!("get_sse_fixture returned different values"),
        }
    }
}
