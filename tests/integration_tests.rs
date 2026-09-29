use axum::{
    extract::Path,
    routing::{get, put},
    Json, Router,
};
use reqwest::Client;
use rust_cache_server::{
    api::build_router,
    cache::{CacheEngine, EvictionPolicy, TtlManager, TtlRule},
    metrics::MetricsRegistry,
    middleware::AppState,
    proxy::BackendForwarder,
};
use serde_json::{json, Value};
use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};

struct MockBackendState {
    pub product_views: AtomicUsize,
}

#[tokio::test]
async fn test_full_gateway_integration() {
    // 1. Start Mock Backend Server
    let backend_state = Arc::new(MockBackendState {
        product_views: AtomicUsize::new(0),
    });

    let backend_state_clone = Arc::clone(&backend_state);
    let backend_app = Router::new()
        .route(
            "/api/products/{id}",
            get({
                let state = Arc::clone(&backend_state_clone);
                move |Path(id): Path<String>| {
                    let state = Arc::clone(&state);
                    async move {
                        state.product_views.fetch_add(1, Ordering::SeqCst);
                        tokio::time::sleep(Duration::from_millis(20)).await;
                        Json(json!({
                            "id": id,
                            "name": "Mechanical Keyboard",
                            "price": 120
                        }))
                    }
                }
            }),
        )
        .route(
            "/api/products/{id}",
            put(|Path(id): Path<String>| async move {
                Json(json!({
                    "id": id,
                    "name": "Updated Keyboard",
                    "price": 150
                }))
            }),
        );

    let backend_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let backend_addr = backend_listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(backend_listener, backend_app).await.unwrap();
    });

    // 2. Start Rust Cache Gateway pointing to mock backend
    let rules = vec![TtlRule::new("/api/products/*", 60)];
    let ttl_manager = TtlManager::new(60, rules);
    let cache = CacheEngine::new(4, 10 * 1024 * 1024, EvictionPolicy::Lru, ttl_manager, None, true);
    let metrics = MetricsRegistry::new();
    let forwarder = BackendForwarder::new(
        format!("http://{}", backend_addr),
        5,
        metrics.clone(),
    );

    let gateway_state = AppState {
        cache,
        forwarder,
        metrics,
        admin_api_key: Some("test-admin-secret".to_string()),
    };

    let gateway_app = build_router(gateway_state);
    let gateway_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let gateway_addr = gateway_listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(gateway_listener, gateway_app).await.unwrap();
    });

    let client = Client::new();
    let gateway_base = format!("http://{}", gateway_addr);

    // --- TEST 1: Cache Miss on first request ---
    let resp1 = client
        .get(format!("{}/api/products/100", gateway_base))
        .send()
        .await
        .unwrap();

    assert_eq!(resp1.status(), 200);
    assert_eq!(resp1.headers().get("x-cache").unwrap(), "MISS");
    assert_eq!(backend_state.product_views.load(Ordering::SeqCst), 1);

    // --- TEST 2: Cache Hit on second request ---
    let resp2 = client
        .get(format!("{}/api/products/100", gateway_base))
        .send()
        .await
        .unwrap();

    assert_eq!(resp2.status(), 200);
    assert_eq!(resp2.headers().get("x-cache").unwrap(), "HIT");
    // Backend counter MUST still be 1!
    assert_eq!(backend_state.product_views.load(Ordering::SeqCst), 1);

    // --- TEST 3: Invalidation via PUT ---
    let put_resp = client
        .put(format!("{}/api/products/100", gateway_base))
        .json(&json!({"name": "Updated Keyboard"}))
        .send()
        .await
        .unwrap();
    assert_eq!(put_resp.status(), 200);

    // Subsequent GET should be a cache MISS and query the backend
    let resp3 = client
        .get(format!("{}/api/products/100", gateway_base))
        .send()
        .await
        .unwrap();
    assert_eq!(resp3.headers().get("x-cache").unwrap(), "MISS");
    assert_eq!(backend_state.product_views.load(Ordering::SeqCst), 2);

    // --- TEST 4: Single-Flight stampede coalescing ---
    let mut handles = vec![];
    for _ in 0..10 {
        let url = format!("{}/api/products/999", gateway_base);
        let c = client.clone();
        handles.push(tokio::spawn(async move {
            c.get(url).send().await.unwrap()
        }));
    }

    for h in handles {
        let r = h.await.unwrap();
        assert_eq!(r.status(), 200);
    }

    // Backend should only have received 1 backend request for product 999!
    // Total backend requests so far: 2 (from previous tests) + 1 (for 999) = 3
    assert_eq!(backend_state.product_views.load(Ordering::SeqCst), 3);

    // --- TEST 5: Health & Admin Stats ---
    let health_resp: Value = client
        .get(format!("{}/health", gateway_base))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(health_resp["status"], "ok");

    let stats_resp: Value = client
        .get(format!("{}/admin/stats", gateway_base))
        .header("x-admin-key", "test-admin-secret")
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(stats_resp["keys"].as_u64().unwrap() > 0);
    assert!(stats_resp["hits"].as_u64().unwrap() >= 1);
}
