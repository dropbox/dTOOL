//! API Route Handlers
//!
//! Organized by resource type: packages, search, contributions, trust, batch.

pub mod batch;
pub mod contributions;
pub mod health;
pub mod metrics;
pub mod packages;
pub mod search;
pub mod trust;

use crate::api::AppState;
use axum::Router;

/// Create the complete API router
pub fn api_router(state: AppState) -> Router {
    // Build nested API routes
    let api_routes = Router::new()
        .nest("/packages", packages::routes())
        .nest("/search", search::routes())
        .nest("/contributions", contributions::routes())
        .nest("/trust", trust::routes())
        .nest("/batch", batch::routes());

    // Build health routes (with state for readiness checks)
    let health = health::health_routes();

    // Build metrics routes (Prometheus endpoint)
    let metrics = metrics::metrics_routes();

    // Combine everything with shared state
    Router::new()
        .nest("/api/v1", api_routes)
        .merge(health)
        .merge(metrics)
        .with_state(state)
}
