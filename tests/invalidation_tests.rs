use bytes::Bytes;
use http::HeaderMap;
use rust_cache_server::cache::{CacheEngine, CacheKey, EvictionPolicy, TtlManager};

#[tokio::test]
async fn test_path_invalidation() {
    let ttl_manager = TtlManager::new(60, vec![]);
    let engine = CacheEngine::new(4, 1024 * 1024, EvictionPolicy::Lru, ttl_manager, None, true);

    let key1 = CacheKey::new("GET", "/api/users/100", None);
    let key2 = CacheKey::new("GET", "/api/users/100", Some("fields=name"));
    let key3 = CacheKey::new("GET", "/api/users/200", None);

    let headers = HeaderMap::new();
    let body = Bytes::from("{\"id\": 100}");

    engine.set(&key1, "/api/users/100", 200, &headers, body.clone(), None);
    engine.set(&key2, "/api/users/100", 200, &headers, body.clone(), None);
    engine.set(&key3, "/api/users/200", 200, &headers, body.clone(), None);

    assert!(engine.get(&key1).is_some());
    assert!(engine.get(&key2).is_some());
    assert!(engine.get(&key3).is_some());

    // Invalidate /api/users/100
    let invalidated = engine.invalidate_path("/api/users/100");
    assert_eq!(invalidated, 2);

    assert!(engine.get(&key1).is_none());
    assert!(engine.get(&key2).is_none());
    assert!(engine.get(&key3).is_some());
}
