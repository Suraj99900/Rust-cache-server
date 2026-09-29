mod api;
mod cache;
mod config;
mod errors;
mod metrics;
mod middleware;
mod proxy;

use std::net::SocketAddr;
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use api::build_router;
use cache::{CacheEngine, EvictionPolicy, TtlManager, TtlRule};
use config::Settings;
use metrics::MetricsRegistry;
use middleware::AppState;
use proxy::BackendForwarder;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize structured logging
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,rust_cache_server=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer().json())
        .init();

    // Load configuration
    let settings = Settings::load();
    info!(
        backend_url = %settings.backend.url,
        port = settings.server.port,
        cache_enabled = settings.cache.enabled,
        max_memory_mb = settings.cache.max_memory_mb,
        shards = settings.cache.shards,
        "Starting Rust Cache Gateway"
    );

    // Initialize TTL Manager
    let rules = settings
        .cache
        .rules
        .iter()
        .map(|r| TtlRule::new(&r.pattern, r.ttl_seconds))
        .collect();
    let ttl_manager = TtlManager::new(settings.cache.default_ttl_seconds, rules);

    // Initialize Eviction Policy
    let eviction_policy = match settings.cache.eviction_policy.to_lowercase().as_str() {
        "lru" => EvictionPolicy::Lru,
        _ => EvictionPolicy::TtlOnly,
    };

    let max_memory_bytes = settings.cache.max_memory_mb * 1024 * 1024;

    // Initialize Cache Engine
    let cache = CacheEngine::new(
        settings.cache.shards,
        max_memory_bytes,
        eviction_policy,
        ttl_manager,
        settings.cache.negative_ttl_seconds,
        settings.cache.enabled,
    );

    // Initialize Metrics & Forwarder
    let metrics = MetricsRegistry::new();
    let forwarder = BackendForwarder::new(
        settings.backend.url.clone(),
        settings.backend.timeout_seconds,
        metrics.clone(),
    );

    let state = AppState {
        cache,
        forwarder,
        metrics,
        admin_api_key: settings.server.admin_api_key,
    };

    let app = build_router(state);

    let addr: SocketAddr = format!("{}:{}", settings.server.host, settings.server.port)
        .parse()
        .expect("Invalid server host or port");

    info!("Rust Cache Gateway listening on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
