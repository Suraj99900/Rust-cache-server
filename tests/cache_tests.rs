use bytes::Bytes;
use http::HeaderMap;
use rust_cache_server::cache::{CacheEngine, CacheKey, EvictionPolicy, TtlManager};
use std::time::Duration;

#[tokio::test]
async fn test_cache_set_get_delete() {
    let ttl_manager = TtlManager::new(60, vec![]);
    let engine = CacheEngine::new(4, 1024 * 1024, EvictionPolicy::Lru, ttl_manager, None, true);

    let key = CacheKey::new("GET", "/api/users/1", None, None, None);
    let body = Bytes::from("{\"user_id\": 1}");
    let headers = HeaderMap::new();

    // 1. Initially missing
    assert!(engine.get(&key).is_none());

    // 2. SET
    engine.set(&key, "/api/users/1", 200, &headers, body.clone(), None);

    // 3. GET (Hit)
    let cached = engine.get(&key).expect("Should find cached item");
    assert_eq!(cached.status, 200);
    assert_eq!(cached.body, body);

    // 4. DELETE
    assert!(engine.delete(&key));
    assert!(engine.get(&key).is_none());
}

#[tokio::test]
async fn test_cache_expiration() {
    let ttl_manager = TtlManager::new(1, vec![]); // 1 sec default
    let engine = CacheEngine::new(4, 1024 * 1024, EvictionPolicy::Lru, ttl_manager, None, true);

    let key = CacheKey::new("GET", "/api/temp", None, None, None);
    let body = Bytes::from("temp data");
    let headers = HeaderMap::new();

    // Set with custom 100ms TTL
    engine.set(
        &key,
        "/api/temp",
        200,
        &headers,
        body,
        Some(Duration::from_millis(100)),
    );

    assert!(engine.get(&key).is_some());

    // Wait for expiration
    tokio::time::sleep(Duration::from_millis(150)).await;

    assert!(engine.get(&key).is_none());
}

#[tokio::test]
async fn test_lru_eviction() {
    // 1 shard, memory limit 600 bytes
    let ttl_manager = TtlManager::new(60, vec![]);
    let engine = CacheEngine::new(1, 600, EvictionPolicy::Lru, ttl_manager, None, true);

    let headers = HeaderMap::new();
    let body = Bytes::from("a".repeat(100)); // ~188 bytes each with struct overhead

    let k1 = CacheKey::new("GET", "/item1", None, None, None);
    let k2 = CacheKey::new("GET", "/item2", None, None, None);
    let k3 = CacheKey::new("GET", "/item3", None, None, None);

    engine.set(&k1, "/item1", 200, &headers, body.clone(), None);
    engine.set(&k2, "/item2", 200, &headers, body.clone(), None);

    // Both should fit
    assert!(engine.get(&k1).is_some(), "k1 should fit");
    assert!(engine.get(&k2).is_some(), "k2 should fit");

    // Access k1 to make k2 the least recently used
    let _ = engine.get(&k1);

    // Insert k3 -> should evict k2
    engine.set(&k3, "/item3", 200, &headers, body.clone(), None);

    assert!(engine.get(&k1).is_some(), "k1 should still exist");
    assert!(engine.get(&k3).is_some(), "k3 should exist");
    assert!(engine.get(&k2).is_none(), "k2 should have been evicted by LRU");
}

#[tokio::test]
async fn test_cache_stats() {
    let ttl_manager = TtlManager::new(60, vec![]);
    let engine = CacheEngine::new(2, 1024 * 1024, EvictionPolicy::Lru, ttl_manager, None, true);

    let key = CacheKey::new("GET", "/test", None, None, None);
    let _ = engine.get(&key); // Miss

    let headers = HeaderMap::new();
    engine.set(&key, "/test", 200, &headers, Bytes::from("hello"), None);
    let _ = engine.get(&key); // Hit

    let stats = engine.stats();
    assert_eq!(stats.hits, 1);
    assert_eq!(stats.misses, 1);
    assert_eq!(stats.sets, 1);
    assert_eq!(stats.keys, 1);
    assert!(stats.memory_bytes > 0);
    assert_eq!(stats.hit_rate, 50.0);
}
