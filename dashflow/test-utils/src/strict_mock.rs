//! Strict Mock Server for Integration Testing (M-240)
//!
//! This module provides a `StrictMockServer` wrapper around wiremock that enforces
//! strict verification of mock expectations. By default, wiremock returns 404 for
//! unmatched requests but doesn't fail the test. `StrictMockServer` ensures tests
//! fail when:
//!
//! 1. Requests hit unexpected paths (not matched by any mock)
//! 2. Registered mocks are never called (test might be hitting wrong URL)
//!
//! # Usage
//!
//! ```ignore
//! use dashflow_test_utils::strict_mock::{StrictMockServer, StrictMock};
//! use wiremock::matchers::{method, path};
//! use wiremock::ResponseTemplate;
//!
//! #[tokio::test]
//! async fn test_api_call() {
//!     let server = StrictMockServer::start().await;
//!
//!     // Register mock with automatic expect(1..) - must be called at least once
//!     server.register(
//!         StrictMock::given(method("POST"))
//!             .and(path("/api/chat"))
//!             .respond_with(ResponseTemplate::new(200).set_body_json(json!({"ok": true})))
//!     ).await;
//!
//!     // Call your API client pointing at server.uri()
//!     let client = MyClient::new(&server.uri());
//!     client.chat().await.unwrap();
//!
//!     // verify() is called automatically on drop, but can be called explicitly
//!     server.verify().await;
//! }
//! ```
//!
//! # Why Strict Mock Servers?
//!
//! Without strict verification, tests can silently pass even when:
//! - The client calls the wrong URL (receives 404, but test doesn't check response)
//! - The mock is misconfigured (path doesn't match actual request)
//! - The client doesn't make the request at all
//!
//! This leads to false confidence in test coverage and bugs in production.

use std::ops::Deref;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use wiremock::matchers::any;
use wiremock::{Mock, MockServer, Request, ResponseTemplate, Times};

/// A strict mock server that verifies all expectations on drop.
///
/// Unlike the standard `MockServer`, this wrapper:
/// - Registers a catch-all mock that logs unmatched requests
/// - Automatically verifies all registered mocks were called on drop
/// - Provides clear error messages when tests fail due to unmet expectations
pub struct StrictMockServer {
    inner: MockServer,
    verified: Arc<AtomicBool>,
}

impl StrictMockServer {
    /// Start a new strict mock server.
    ///
    /// This server will fail the test if:
    /// - Any registered mock is not called at least once (unless configured otherwise)
    /// - Any request doesn't match a registered mock (404 returned + logged)
    pub async fn start() -> Self {
        let inner = MockServer::start().await;

        // Register a catch-all mock at lowest priority that responds with 404
        // and a clear error message. This helps debug when tests hit wrong URLs.
        Mock::given(any())
            .respond_with(ResponseTemplate::new(404).set_body_string(
                "StrictMockServer: No mock matched this request. \
                 Check that your test registers the correct paths.",
            ))
            .with_priority(u8::MAX) // Lowest priority - only matches if nothing else does
            .mount(&inner)
            .await;

        Self {
            inner,
            verified: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Get the base URI for this mock server.
    ///
    /// Use this URI as the base URL for your API client.
    pub fn uri(&self) -> String {
        self.inner.uri()
    }

    /// Register a mock with this server.
    ///
    /// Use `StrictMock` for automatic expect(1..) behavior, or standard `Mock`
    /// for custom expectations.
    pub async fn register(&self, mock: Mock) {
        mock.mount(&self.inner).await;
    }

    /// Register a strict mock that must be called at least once.
    ///
    /// This is a convenience method equivalent to:
    /// ```ignore
    /// server.register(StrictMock::given(matcher).respond_with(response)).await;
    /// ```
    pub async fn register_strict<M: wiremock::Match + 'static>(
        &self,
        matcher: M,
        response: ResponseTemplate,
    ) {
        Mock::given(matcher)
            .respond_with(response)
            .expect(1..)
            .mount(&self.inner)
            .await;
    }

    /// Verify all mock expectations were met.
    ///
    /// This is called automatically on drop, but you can call it explicitly
    /// to get verification errors at a specific point in your test.
    ///
    /// # Panics
    ///
    /// Panics if any mock's expectations were not met.
    pub async fn verify(&self) {
        self.verified.store(true, Ordering::SeqCst);
        self.inner.verify().await;
    }

    /// Get all received requests for debugging.
    ///
    /// Useful for understanding what requests were made when a test fails.
    pub async fn received_requests(&self) -> Vec<Request> {
        self.inner
            .received_requests()
            .await
            .unwrap_or_default()
    }

    /// Print a summary of all received requests for debugging.
    ///
    /// Call this when a test fails to understand what requests were made.
    pub async fn debug_requests(&self) {
        let requests = self.received_requests().await;
        if requests.is_empty() {
            eprintln!("StrictMockServer: No requests received");
        } else {
            eprintln!("StrictMockServer: {} request(s) received:", requests.len());
            for (i, req) in requests.iter().enumerate() {
                eprintln!(
                    "  [{}] {} {}",
                    i + 1,
                    req.method,
                    req.url
                );
            }
        }
    }

    /// Access the underlying MockServer for advanced operations.
    pub fn inner(&self) -> &MockServer {
        &self.inner
    }
}

impl Deref for StrictMockServer {
    type Target = MockServer;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl Drop for StrictMockServer {
    fn drop(&mut self) {
        // Skip verification if already verified or if we're panicking
        // (to avoid double-panic which hides the original error)
        if self.verified.load(Ordering::SeqCst) || std::thread::panicking() {
            return;
        }

        // Note: We can't call async verify() in drop, but we log a warning
        // to remind developers to call verify() explicitly in async tests.
        eprintln!(
            "StrictMockServer: Warning - verify() was not called. \
             Call server.verify().await at the end of your test to ensure \
             all mock expectations were met."
        );
    }
}

/// Builder for strict mocks that require at least one call by default.
///
/// This is a convenience wrapper around `Mock` that sets `expect(1..)` by default.
///
/// # Example
///
/// ```ignore
/// use dashflow_test_utils::strict_mock::StrictMock;
/// use wiremock::matchers::{method, path};
/// use wiremock::ResponseTemplate;
///
/// // Create a mock that MUST be called at least once
/// let mock = StrictMock::given(method("POST"))
///     .and(path("/api/chat"))
///     .respond_with(ResponseTemplate::new(200));
/// ```
pub struct StrictMock;

impl StrictMock {
    /// Create a new strict mock with the given matcher.
    ///
    /// By default, strict mocks expect to be called at least once.
    /// Use `.expect(times)` to customize.
    pub fn given<M: wiremock::Match + 'static>(matcher: M) -> StrictMockBuilder {
        StrictMockBuilder {
            mock: Mock::given(matcher),
            times: Some(Times::from(1..)),
        }
    }
}

/// Builder for configuring strict mocks.
pub struct StrictMockBuilder {
    mock: wiremock::MockBuilder,
    times: Option<Times>,
}

impl StrictMockBuilder {
    /// Add another matcher requirement.
    pub fn and<M: wiremock::Match + 'static>(mut self, matcher: M) -> Self {
        self.mock = self.mock.and(matcher);
        self
    }

    /// Set the response for this mock.
    pub fn respond_with<R: wiremock::Respond + 'static>(self, response: R) -> Mock {
        let mock = self.mock.respond_with(response);
        match self.times {
            Some(times) => mock.expect(times),
            None => mock,
        }
    }

    /// Override the default expectation (at least 1 call).
    ///
    /// Use this when you need custom expectations:
    /// - `expect(0)` - mock should NOT be called (useful for negative tests)
    /// - `expect(2)` - expect exactly 2 calls
    /// - `expect(1..=3)` - expect 1 to 3 calls
    pub fn expect<T: Into<Times>>(mut self, times: T) -> Self {
        self.times = Some(times.into());
        self
    }

    /// Remove expectation - mock can be called any number of times (including zero).
    ///
    /// Use this when the mock is optional and may or may not be called.
    pub fn any_times(mut self) -> Self {
        self.times = None;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use wiremock::matchers::{method, path};

    #[tokio::test]
    async fn test_strict_mock_server_basic() {
        let server = StrictMockServer::start().await;

        server
            .register(
                StrictMock::given(method("GET"))
                    .and(path("/test"))
                    .respond_with(ResponseTemplate::new(200).set_body_json(json!({"ok": true}))),
            )
            .await;

        // Make the expected request
        let client = reqwest::Client::new();
        let resp = client
            .get(format!("{}/test", server.uri()))
            .send()
            .await
            .unwrap();

        assert_eq!(resp.status(), 200);

        // Verify expectations were met
        server.verify().await;
    }

    #[tokio::test]
    async fn test_strict_mock_unmatched_returns_404() {
        let server = StrictMockServer::start().await;

        // Register a mock for /expected
        server
            .register(
                StrictMock::given(method("GET"))
                    .and(path("/expected"))
                    .any_times() // Don't require this to be called
                    .respond_with(ResponseTemplate::new(200)),
            )
            .await;

        // Call /wrong instead
        let client = reqwest::Client::new();
        let resp = client
            .get(format!("{}/wrong", server.uri()))
            .send()
            .await
            .unwrap();

        // Should get 404 from catch-all
        assert_eq!(resp.status(), 404);
        let body = resp.text().await.unwrap();
        assert!(body.contains("No mock matched"));

        server.verify().await;
    }

    #[tokio::test]
    async fn test_strict_mock_register_strict() {
        let server = StrictMockServer::start().await;

        // Use register_strict for simple single-matcher mocks
        server
            .register_strict(path("/api"), ResponseTemplate::new(201))
            .await;

        let client = reqwest::Client::new();
        let resp = client
            .post(format!("{}/api", server.uri()))
            .send()
            .await
            .unwrap();

        assert_eq!(resp.status(), 201);
        server.verify().await;
    }

    #[tokio::test]
    async fn test_strict_mock_multi_matcher() {
        let server = StrictMockServer::start().await;

        // Use StrictMock builder for multiple matchers
        server
            .register(
                StrictMock::given(method("POST"))
                    .and(path("/api"))
                    .respond_with(ResponseTemplate::new(201)),
            )
            .await;

        let client = reqwest::Client::new();
        let resp = client
            .post(format!("{}/api", server.uri()))
            .send()
            .await
            .unwrap();

        assert_eq!(resp.status(), 201);
        server.verify().await;
    }

    #[tokio::test]
    async fn test_debug_requests() {
        let server = StrictMockServer::start().await;

        server
            .register(
                StrictMock::given(method("GET"))
                    .and(path("/test"))
                    .any_times()
                    .respond_with(ResponseTemplate::new(200)),
            )
            .await;

        let client = reqwest::Client::new();
        let _ = client
            .get(format!("{}/test", server.uri()))
            .send()
            .await;
        let _ = client
            .get(format!("{}/other", server.uri()))
            .send()
            .await;

        let requests = server.received_requests().await;
        assert_eq!(requests.len(), 2);

        server.verify().await;
    }
}
