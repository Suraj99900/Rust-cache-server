pub mod admin;
pub mod health;

use axum::{
    routing::{delete, get},
    Router,
};

use crate::middleware::{proxy_handler, AppState};

pub fn build_router(state: AppState) -> Router {
    Router::new()
        // Observability & Admin routes
        .route("/health", get(health::health_check))
        .route("/metrics", get(admin::prometheus_metrics))
        .route("/admin/stats", get(admin::admin_stats))
        .route("/admin/cache", delete(admin::admin_purge_all))
        .route("/admin/cache/{key}", delete(admin::admin_purge_key))
        // Catch-all Reverse Proxy route for all other traffic and methods
        .fallback(proxy_handler)
        .with_state(state)
}
