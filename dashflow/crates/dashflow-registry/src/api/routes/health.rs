//! Health Check Routes
//!
//! Endpoints for service health and readiness checks.
//!
//! - `/health` - Basic liveness check (is the service running?)
//! - `/ready` - Readiness check (is the service ready to accept requests?)
//!
//! The readiness check verifies database connectivity and other dependencies.

use axum::{extract::State, http::StatusCode, routing::get, Json, Router};
use serde::Serialize;
use tracing::{debug, warn};

use crate::api::state::AppState;

/// Health routes (at root level)
pub fn health_routes() -> Router<AppState> {
    Router::new()
        .route("/health", get(health_check))
        .route("/ready", get(readiness_check))
        .route("/", get(root))
}

/// Root endpoint - basic info
async fn root() -> Json<ServiceInfo> {
    Json(ServiceInfo {
        name: "dashflow-registry",
        version: env!("CARGO_PKG_VERSION"),
        description: "AI-native package registry",
    })
}

/// Service info response
#[derive(Serialize)]
struct ServiceInfo {
    name: &'static str,
    version: &'static str,
    description: &'static str,
}

/// Health check - is the service running?
/// This is a simple liveness probe that always succeeds if the server is up.
async fn health_check() -> Json<HealthStatus> {
    Json(HealthStatus {
        status: "healthy",
        timestamp: chrono::Utc::now(),
    })
}

/// Health status response
#[derive(Serialize)]
struct HealthStatus {
    status: &'static str,
    timestamp: chrono::DateTime<chrono::Utc>,
}

/// Readiness check - is the service ready to accept requests?
/// Verifies database connectivity and other dependencies.
async fn readiness_check(
    State(state): State<AppState>,
) -> Result<Json<ReadinessStatus>, (StatusCode, Json<ReadinessStatus>)> {
    let mut checks = ReadinessChecks {
        database: false,
        cache: false,
        search: false,
    };
    let mut errors: Vec<String> = Vec::new();

    // Check database by attempting to check if a name exists
    // This exercises the database connection pool
    match state.metadata.name_exists("__health_check__").await {
        Ok(_) => {
            checks.database = true;
            debug!("Database health check passed");
        }
        Err(e) => {
            let error_msg = format!("Database check failed: {}", e);
            warn!("{}", error_msg);
            errors.push(error_msg);
        }
    }

    // Check cache by verifying we can check key existence
    // This exercises the cache connection (Redis, in-memory, etc.)
    match state.data_cache.exists("__health_check__").await {
        Ok(_) => {
            checks.cache = true;
            debug!("Cache health check passed");
        }
        Err(e) => {
            let error_msg = format!("Cache check failed: {}", e);
            warn!("{}", error_msg);
            errors.push(error_msg);
        }
    }

    // Check search by verifying we can execute a search query
    // This exercises the search service (vector store, embeddings, etc.)
    match state.search.search("__health_check__", 1).await {
        Ok(_) => {
            checks.search = true;
            debug!("Search health check passed");
        }
        Err(e) => {
            let error_msg = format!("Search check failed: {}", e);
            warn!("{}", error_msg);
            errors.push(error_msg);
        }
    }

    let ready = checks.database && checks.cache && checks.search;

    let status = ReadinessStatus {
        ready,
        checks,
        errors: if errors.is_empty() {
            None
        } else {
            Some(errors)
        },
    };

    if ready {
        Ok(Json(status))
    } else {
        Err((StatusCode::SERVICE_UNAVAILABLE, Json(status)))
    }
}

/// Readiness status response
#[derive(Debug, Serialize)]
struct ReadinessStatus {
    ready: bool,
    checks: ReadinessChecks,
    #[serde(skip_serializing_if = "Option::is_none")]
    errors: Option<Vec<String>>,
}

/// Individual readiness checks
#[derive(Debug, Serialize)]
struct ReadinessChecks {
    database: bool,
    cache: bool,
    search: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_health_check() {
        let response = health_check().await;
        assert_eq!(response.status, "healthy");
    }

    #[tokio::test]
    async fn test_readiness_check_with_state() {
        let state = AppState::new().await.unwrap();
        let result = readiness_check(State(state)).await;
        assert!(result.is_ok());
        let status = result.unwrap();
        assert!(status.ready);
        assert!(status.checks.database);
        assert!(status.checks.cache);
        assert!(status.checks.search);
    }
}
